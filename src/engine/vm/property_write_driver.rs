//! Transfer assignment inputs once, then let the object protocol own the write.
use super::{
    Completion, driver::CallStep, exception::runtime_error_to_vm_error,
    execution::RunningExecution, frame::FrameId,
};
use crate::engine::{
    api::{Error, ErrorKind, runtime::Runtime},
    object::PropertyKey,
    value::{Value, conversion::NativeConversion},
};

pub(super) struct ConvertedWrite {
    pub base: Value,
    pub key: Value,
    pub value: Value,
}

pub(super) fn write(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    static_key: Option<u32>,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let realm = parent.executable.realm;
    let depth = execution.slots.depth(&parent.window);
    let key = if let Some(index) = static_key {
        let atom = parent
            .executable
            .property_key_atoms
            .as_ref()
            .and_then(|atoms| atoms.get(index as usize))
            .copied()
            .filter(|atom| !atom.is_null())
            .ok_or_else(|| Error::internal("property write has no linked key"))?;
        PropertyKey::from_borrowed_atom(runtime.clone(), atom)
            .map_err(|error| Error::internal(error.to_string()))?
    } else {
        let value = execution.slots.peek(&parent.window, 1)?.clone();
        if matches!(value, Value::Object(_)) {
            return Err(Error::internal("object write key did not enter conversion"));
        }
        match runtime
            .native_to_property_key(realm, value)
            .map_err(runtime_error_to_vm_error)?
        {
            NativeConversion::Value(key) => key,
            NativeConversion::Throw(value) => {
                return Ok(CallStep::Complete(Completion::Throw(value)));
            }
        }
    };
    let value = execution.slots.pop(&mut parent.window)?;
    if static_key.is_none() {
        execution.slots.pop(&mut parent.window)?;
    }
    let base = execution.slots.pop(&mut parent.window)?;
    dispatch(runtime, execution, frame, base, key, value, depth)
}

pub(super) fn converted(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    input: Box<ConvertedWrite>,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let realm = parent.executable.realm;
    let depth = execution.slots.depth(&parent.window) + 3;
    let ConvertedWrite { base, key, value } = *input;
    if matches!(key, Value::Object(_)) {
        return Err(Error::internal("write key conversion returned an object"));
    }
    let key = match runtime
        .native_to_property_key(realm, key)
        .map_err(runtime_error_to_vm_error)?
    {
        NativeConversion::Value(key) => key,
        NativeConversion::Throw(value) => return Ok(CallStep::Complete(Completion::Throw(value))),
    };
    dispatch(runtime, execution, frame, base, key, value, depth)
}

fn dispatch(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    base: Value,
    key: PropertyKey,
    value: Value,
    depth: usize,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let realm = parent.executable.realm;
    let strict = parent.executable.metadata.strict;
    let object = match &base {
        Value::Object(object) => object.clone(),
        Value::Null | Value::Undefined => {
            let suffix = if matches!(base, Value::Null) {
                "' of null"
            } else {
                "' of undefined"
            };
            let error = runtime
                .native_atom_error(ErrorKind::Type, "cannot set property '", &key, suffix)
                .map_err(runtime_error_to_vm_error)?;
            return super::property_driver::throw_error(runtime, realm, error);
        }
        value => {
            use crate::engine::builtins::native::PrimitiveKind;
            let kind = match value {
                Value::Bool(_) => PrimitiveKind::Boolean,
                Value::Int(_) | Value::Float(_) => PrimitiveKind::Number,
                Value::String(_) => PrimitiveKind::String,
                Value::BigInt(_) => PrimitiveKind::BigInt,
                Value::Symbol(_) => PrimitiveKind::Symbol,
                _ => unreachable!(),
            };
            runtime
                .primitive_prototype_for_realm(realm, kind)
                .map_err(runtime_error_to_vm_error)?
        }
    };
    super::proxy_get_driver::start_write(
        runtime, execution, frame, object, key, value, base, strict, depth,
    )
}
