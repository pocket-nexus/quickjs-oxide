//! The ordinary call/return loop's only entry and destruction transactions.
use crate::engine::{
    api::{Error, runtime::Runtime},
    vm::{
        call::ordinary::OrdinaryCall,
        exception::runtime_error_to_vm_error,
        execution::RunningExecution,
        frame::{FrameId, ReturnValue},
    },
};

pub(super) fn enter(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    count: u16,
    method: bool,
    tail: bool,
) -> Result<bool, Error> {
    let frame = execution.frames.current_mut(id)?;
    let count = usize::from(count);
    execution
        .slots
        .peek(&frame.window, count + usize::from(method))?;
    let selected = OrdinaryCall::select(runtime, execution.slots.peek(&frame.window, count)?);
    if matches!(selected, Ok(None)) {
        return Ok(false);
    }
    if !execution
        .slots
        .validate_call_value_domains(&frame.window, runtime, count, method)?
    {
        return Ok(false);
    }
    let Some(selected) = selected.map_err(runtime_error_to_vm_error)? else {
        return Ok(false);
    };
    let call = selected
        .authenticate(runtime)
        .map_err(runtime_error_to_vm_error)?;
    if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
        return Ok(false);
    }
    #[cfg(feature = "profiling")]
    let depth = execution
        .slots
        .depth(&execution.frames.current_mut(id)?.window);
    call.install(runtime, execution, id, count, method, tail)?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    Ok(true)
}

pub(super) fn finish(execution: &mut RunningExecution, id: FrameId) -> Result<bool, Error> {
    let frame = execution.frames.current_mut(id)?;
    let Some(target) = frame.cold.ordinary_return() else {
        return Ok(false);
    };
    if execution.pending.is_none() {
        return Ok(false);
    }
    // Result ownership precedes window clearing and activation removal.
    let value = execution.pending.take().unwrap();
    let mut frame = execution.frames.pop(id)?;
    let guard = frame.cold.entry_guard.take();
    execution.slots.clear_frame(frame.window)?;
    if let Some(guard) = guard {
        guard.finish().map_err(runtime_error_to_vm_error)?;
    }
    execution.call_storage.recycle(frame.cold);
    let parent = execution.frames.current_mut(target.frame()?)?;
    if matches!(target.value_use, ReturnValue::Push) {
        execution.slots.push(&mut parent.window, value)?;
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("ordinary_return_direct");
    Ok(true)
}
