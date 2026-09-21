//! Canonical VM property keys: never repeat user-observable coercion.
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::object::PropertyKey;
use crate::engine::value::JsValue;

pub(super) fn canonical(
    runtime: &Runtime,
    value: &crate::engine::value::JsValue,
) -> Result<PropertyKey, Error> {
    if let Some(key) = runtime.immediate_numeric_property_key_jsvalue(value) {
        return Ok(key);
    }
    match value {
        crate::engine::value::JsValue::Symbol(index) => {
            let atom = runtime
                .0
                .state
                .borrow()
                .atoms
                .brand(*index)
                .map_err(|error| Error::internal(error.to_string()))?;
            PropertyKey::from_borrowed_atom(runtime.clone(), atom)
                .map_err(|error| Error::internal(error.to_string()))
        }
        crate::engine::value::JsValue::String(id) => {
            let string = runtime
                .0
                .state
                .borrow()
                .heap
                .string(*id)
                .map_err(|error| Error::internal(error.to_string()))?
                .clone();
            runtime
                .intern_property_key_js_string(&string)
                .map_err(|error| Error::internal(error.to_string()))
        }
        JsValue::Int(value) => runtime
            .intern_property_key_js_string(&super::to_js_string_jsvalue(
                runtime,
                &JsValue::Int(*value),
            )?)
            .map_err(|error| Error::internal(error.to_string())),
        _ => Err(Error::internal(
            "computed property key was not canonicalized by ToPropKey",
        )),
    }
}

/// Name inference consumes a canonical key, including Symbol descriptions.
pub(super) fn computed_name(
    runtime: &Runtime,
    key: &JsValue,
) -> Result<crate::engine::value::JsString, Error> {
    use crate::engine::value::JsString;
    Ok(match key {
        JsValue::Int(_) => super::to_js_string_jsvalue(runtime, key)?,
        JsValue::String(id) => runtime
            .0
            .state
            .borrow()
            .heap
            .string(*id)
            .map_err(|error| Error::internal(error.to_string()))?
            .clone(),
        JsValue::Symbol(index) => {
            let description = {
                let state = runtime.0.state.borrow();
                let atom = state
                    .atoms
                    .brand(*index)
                    .map_err(|error| Error::internal(error.to_string()))?;
                let info = state
                    .atoms
                    .resolve(atom)
                    .map_err(|error| Error::internal(error.to_string()))?;
                match info.spelling {
                    crate::engine::atom::AtomSpelling::Text(text) => Some(text.clone()),
                    crate::engine::atom::AtomSpelling::NoDescription => None,
                    _ => {
                        return Err(Error::internal(
                            "symbol atom had an immediate-integer spelling",
                        ));
                    }
                }
            };
            match description {
                None => JsString::from_static(""),
                Some(description) => JsString::from_static("[")
                    .try_concat(&description)?
                    .try_concat(&JsString::from_static("]"))?,
            }
        }
        _ => {
            return Err(Error::internal(
                "computed function name was not a canonical property key",
            ));
        }
    })
}

#[inline(never)]
pub(super) fn set_name(
    runtime: &Runtime,
    execution: &mut super::execution::RunningExecution,
    id: super::frame::FrameId,
    index: Option<u32>,
) -> Result<Option<JsValue>, Error> {
    use super::exception::runtime_error_to_vm_error;
    let frame = execution.frames.current_mut(id)?;
    let result = (|| -> Result<(), Error> {
        let name = match index {
            Some(index) => {
                let Some(crate::engine::heap::BytecodeConstant::Value(
                    crate::engine::heap::RawValue::String(name),
                )) = frame.executable.constant(index)
                else {
                    return Err(Error::internal(
                        "function-name opcode referenced a non-string constant",
                    ));
                };
                // The published bytecode node owns the constant-pool edge, so
                // the trusted read clones the payload Rc without retaining.
                runtime.0.state.borrow().heap.string_fast(*name).clone()
            }
            None => computed_name(runtime, execution.slots.peek(&frame.window, 1)?)?,
        };
        let JsValue::Object(target) = execution.slots.peek(&frame.window, 0)? else {
            return Ok(());
        };
        let target =
            crate::engine::object::ObjectRef::from_borrowed_handle(runtime.clone(), *target)
                .map_err(|error| runtime_error_to_vm_error(error.into()))?;
        runtime
            .define_object_name_for_object(&target, &name)
            .map_err(runtime_error_to_vm_error)
    })();
    match result {
        Ok(()) => {
            frame.resume_pc = frame
                .fault_pc
                .checked_add(1)
                .ok_or_else(|| Error::internal("name resume PC overflow"))?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_instruction(
                execution.slots.depth(&frame.window),
            );
            Ok(None)
        }
        Err(error) => {
            let Some(kind) =
                crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
            else {
                return Err(error);
            };
            Ok(Some(
                runtime
                    .new_native_error_from_error_jsvalue(frame.executable.realm, kind, &error)
                    .map_err(runtime_error_to_vm_error)?,
            ))
        }
    }
}
