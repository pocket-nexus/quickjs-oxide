//! Array literal writes retain their private index for subsequent append steps.
use super::{
    Completion, driver::CallStep, exception::runtime_error_to_vm_error,
    execution::RunningExecution, frame::FrameId,
};
use crate::engine::{
    api::{Error, runtime::Runtime},
    heap::{ObjectKind, ObjectPayload},
    object::{DescriptorField, OrdinaryPropertyDescriptor, operations::PropertyDefineOutcome},
    value::{Value, conversion::NativeConversion},
};

#[inline(never)]
pub(super) fn define_element(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let Value::Object(object) = execution.slots.peek(&frame.window, 2)? else {
        return Ok(CallStep::Bridge);
    };
    if !object.belongs_to(runtime) {
        return Ok(CallStep::Bridge);
    }
    {
        let state = runtime.0.state.borrow();
        let data = state
            .heap
            .object(object.object_id())
            .map_err(|e| Error::internal(e.to_string()))?;
        if !matches!(
            (data.kind, &data.payload),
            (ObjectKind::Ordinary, ObjectPayload::Ordinary)
                | (ObjectKind::Array, ObjectPayload::Array { .. })
        ) {
            return Ok(CallStep::Bridge);
        }
    }
    let index = execution.slots.peek(&frame.window, 1)?;
    // Numeric compiler indices cannot select Array's coercing length setter.
    if !matches!(index, Value::Int(_) | Value::Float(_)) {
        return Ok(CallStep::Bridge);
    }
    let key = match runtime
        .native_to_property_key(realm, index.clone())
        .map_err(runtime_error_to_vm_error)?
    {
        NativeConversion::Value(key) => key,
        NativeConversion::Throw(value) => return Ok(CallStep::Complete(Completion::Throw(value))),
    };
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let result = runtime.define_own_property_in_realm(
        Some(realm),
        object,
        &key,
        &OrdinaryPropertyDescriptor {
            value: DescriptorField::Present(execution.slots.peek(&frame.window, 0)?.clone()),
            writable: DescriptorField::Present(true),
            enumerable: DescriptorField::Present(true),
            configurable: DescriptorField::Present(true),
            ..OrdinaryPropertyDescriptor::new()
        },
    );
    execution.slots.pop(&mut frame.window)?;
    let error = match result {
        Ok(PropertyDefineOutcome::Defined(true)) => {
            frame.resume_pc = frame
                .fault_pc
                .checked_add(1)
                .ok_or_else(|| Error::internal("array element resume PC overflow"))?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_instruction(depth);
            return Ok(CallStep::Entered);
        }
        Ok(PropertyDefineOutcome::Throw(value)) => {
            return Ok(CallStep::Complete(Completion::Throw(value)));
        }
        Ok(PropertyDefineOutcome::Defined(false)) => Error::new(
            crate::engine::api::ErrorKind::Type,
            "property is not configurable",
        ),
        Err(error) => runtime_error_to_vm_error(error),
    };
    let Some(kind) =
        crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
    else {
        return Err(error);
    };
    Ok(CallStep::Complete(Completion::Throw(
        runtime
            .new_native_error_from_error(realm, kind, &error)
            .map_err(runtime_error_to_vm_error)?,
    )))
}
