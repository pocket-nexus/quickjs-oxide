//! Arguments/rest allocation after the driver publishes the instruction PC.
//! These constructors do not invoke JS. Mapped parameters keep their cell
//! identity; extra arguments get detached cells and padding stays invisible.

use super::bindings::capture_frame_binding;
use super::exception::runtime_error_to_vm_error;
use super::execution::RunningExecution;
use super::frame::FrameId;
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::code::bytecode::ArgumentsKind;
use crate::engine::code::function::metadata::{
    ClosureSource, ClosureVariable, ClosureVariableKind, ClosureVariableName,
};
use crate::engine::value::JsValue;

pub(super) fn arguments(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    kind: ArgumentsKind,
) -> Result<JsValue, Error> {
    if let Some(object) = arguments_in_state(runtime, execution, id, kind)? {
        return Ok(object);
    }
    let frame = execution.frames.current_mut(id)?;
    let count = execution.slots.actual_argument_count(&frame.window)?;
    let object = match kind {
        ArgumentsKind::Unmapped => {
            let values = execution
                .slots
                .snapshot_actual_arguments(&frame.window, runtime)?;
            runtime.new_unmapped_arguments_object(frame.executable.realm, values)
        }
        ArgumentsKind::Mapped => {
            let mapped = count.min(frame.executable.argument_definitions.len());
            let mut roots = Vec::new();
            roots
                .try_reserve_exact(count)
                .map_err(|_| Error::internal("arguments cells allocation failed"))?;
            for index in 0..mapped {
                let index = u16::try_from(index)
                    .map_err(|_| Error::internal("argument index exceeds u16::MAX"))?;
                roots.push(capture_frame_binding(
                    runtime,
                    execution.slots.parameter_mut(&frame.window, index)?,
                    ClosureVariable {
                        source: ClosureSource::ParentArgument(index),
                        name: ClosureVariableName::None,
                        is_lexical: false,
                        is_const: false,
                        kind: ClosureVariableKind::Normal,
                    },
                )?);
            }
            for value in execution
                .slots
                .snapshot_argument_tail(&frame.window, runtime, mapped)?
            {
                roots.push(
                    runtime
                        .new_var_ref(value, false, false, ClosureVariableKind::Normal)
                        .map_err(runtime_error_to_vm_error)?,
                );
            }
            let function = frame
                .cold
                .function
                .to_root(runtime)
                .map_err(runtime_error_to_vm_error)?;
            runtime.new_mapped_arguments_object(frame.executable.realm, &function, roots)
        }
    }
    .map_err(runtime_error_to_vm_error)?;
    Ok(JsValue::Object(object.into_handle()))
}

/// Build the arguments object from the frame under one state access. Mapped
/// parameters are captured first, so their cells alias the formals; extra
/// actual arguments map to nothing and become plain data, as in the
/// specification. `None` leaves an unusual binding to the general path.
fn arguments_in_state(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    kind: ArgumentsKind,
) -> Result<Option<JsValue>, Error> {
    use crate::engine::heap::PropertySlot;
    let frame = execution.frames.current_mut(id)?;
    let count = execution.slots.actual_argument_count(&frame.window)?;
    let realm = frame.executable.realm;
    let (mapped, callee) = match kind {
        ArgumentsKind::Unmapped => (0, None),
        ArgumentsKind::Mapped => (
            count.min(frame.executable.argument_definitions.len()),
            Some(frame.cold.function.object_id()),
        ),
    };
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(count + 3)
        .map_err(|_| Error::internal("arguments slots allocation failed"))?;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(mapped)
        .map_err(|_| Error::internal("arguments cells allocation failed"))?;
    for index in 0..mapped {
        let index =
            u16::try_from(index).map_err(|_| Error::internal("argument index exceeds u16::MAX"))?;
        let cell = capture_frame_binding(
            runtime,
            execution.slots.parameter_mut(&frame.window, index)?,
            ClosureVariable {
                source: ClosureSource::ParentArgument(index),
                name: ClosureVariableName::None,
                is_lexical: false,
                is_const: false,
                kind: ClosureVariableKind::Normal,
            },
        )?;
        slots.push(PropertySlot::VarRef(cell.id()));
        cells.push(cell);
    }
    let object = {
        let mut state = runtime.0.state.borrow_mut();
        if !execution.slots.argument_data_slots_in_state(
            &frame.window,
            &state,
            mapped,
            &mut slots,
        )? {
            return Ok(None);
        }
        state
            .new_arguments_object_in_state(&runtime.0.poisoned, realm, callee, slots)
            .map_err(runtime_error_to_vm_error)?
    };
    // The object retained each mapped cell; the capture roots end here.
    drop(cells);
    Ok(Some(JsValue::Object(object)))
}

pub(super) fn rest(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    start: u16,
) -> Result<JsValue, Error> {
    let frame = execution.frames.current_mut(id)?;
    let values =
        execution
            .slots
            .snapshot_argument_tail(&frame.window, runtime, usize::from(start))?;
    runtime
        .new_array_from_values_jsvalue(frame.executable.realm, values)
        .map(|object| JsValue::Object(object.into_handle()))
        .map_err(runtime_error_to_vm_error)
}

/// Keep allocation and error materialization out of the driver's resident native
/// frame, including when an unsupported later instruction reenters the host.
#[inline(never)]
pub(super) fn step(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    exit: super::execute::VmAction,
) -> Result<Option<super::Completion>, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let result = match exit {
        super::execute::VmAction::Arguments(kind) => arguments(runtime, execution, id, kind),
        super::execute::VmAction::Rest(start) => rest(runtime, execution, id, start),
        _ => return Err(Error::internal("arguments step received an unrelated exit")),
    };
    match result {
        Ok(value) => {
            let frame = execution.frames.current_mut(id)?;
            execution.slots.push(&mut frame.window, value)?;
            frame.resume_pc = frame.next_pc()?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_instruction(depth);
            Ok(None)
        }
        Err(error) => {
            let Some(kind) =
                crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
            else {
                return Err(error);
            };
            Ok(Some(super::Completion::Throw(
                runtime
                    .new_native_error_from_error_jsvalue(realm, kind, &error)
                    .map_err(runtime_error_to_vm_error)?,
            )))
        }
    }
}
