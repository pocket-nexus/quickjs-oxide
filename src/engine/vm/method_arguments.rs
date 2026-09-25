//! Direct arguments shared by the canonical linked read and resident IC span.
use super::bindings::FrameBinding;
use super::stack::{RunSlots, copy_value};
use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::code::bytecode::Instruction;
use crate::engine::value::JsValue;

/// Read each argument once. A non-direct binding resumes at its own canonical
/// PC after the preceding arguments have already been pushed, exactly as
/// ordinary instruction-by-instruction execution would do.
pub(super) fn argument(
    runtime: &Runtime,
    slots: &RunSlots<'_>,
    instruction: &Instruction,
) -> Result<Option<JsValue>, Error> {
    Ok(Some(match instruction {
        Instruction::GetLocal(index) | Instruction::GetLocalCheck(index) => {
            let Ok(FrameBinding::Direct(value)) = slots.local(*index) else {
                return Ok(None);
            };
            copy_value(runtime, value)?
        }
        Instruction::GetArg(index) => {
            let Ok(FrameBinding::Direct(value)) = slots.parameter(*index) else {
                return Ok(None);
            };
            copy_value(runtime, value)?
        }
        Instruction::PushI32(value) => JsValue::Int(*value),
        Instruction::Undefined => JsValue::Undefined,
        Instruction::Null => JsValue::Null,
        Instruction::PushTrue => JsValue::Bool(true),
        Instruction::PushFalse => JsValue::Bool(false),
        _ => unreachable!("published method call span"),
    }))
}
